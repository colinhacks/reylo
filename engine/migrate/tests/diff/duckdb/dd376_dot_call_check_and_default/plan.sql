create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro m;
create macro m (x) as ((x + 2));
create table t (a integer default m(1), b integer, check ((b.m() > 0)));
insert into t (a, b) select a, b from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
