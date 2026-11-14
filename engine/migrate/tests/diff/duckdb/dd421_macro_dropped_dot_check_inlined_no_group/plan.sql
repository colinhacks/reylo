create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (a integer, check ((a > 0)));
insert into t (a) select a from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop macro d;
