create temporary table reylo_copy_t as select * from s.t;
drop table s.t;
drop macro g;
create macro g (x) as ((x > -1));
create table s.t (a integer, check (g(a)));
insert into s.t (a) select a from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
