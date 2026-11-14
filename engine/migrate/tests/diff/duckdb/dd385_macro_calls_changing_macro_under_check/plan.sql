create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro m1;
create macro m1 (x) as ((x > -5));
create table t (a integer, check (m2(a)));
insert into t (a) select a from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
