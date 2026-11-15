alter table t add column m integer;
create temporary table reylo_copy_u as select * from u;
drop table u;
create table u (x integer, check (((x > 0) and (x < 20))));
insert into u (x) select x from temp.main.reylo_copy_u;
drop table temp.main.reylo_copy_u;
