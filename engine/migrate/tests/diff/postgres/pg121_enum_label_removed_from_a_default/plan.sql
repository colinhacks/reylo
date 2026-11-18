alter type st rename to st__old;
create type st as enum ('a', 'b');
alter table t alter column s drop default;
alter table t alter column s type st using s::text::st;
alter table t alter column s set default 'a'::st;
drop type st__old;
alter table t alter column s set default 'a';
